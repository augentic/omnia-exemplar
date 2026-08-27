#![allow(missing_docs)]
#![cfg(not(miri))]

mod provider;

use std::fs::File;
use std::ops::Sub;

use acme_common::TIMEZONE;
use acme_test::{TestCase, TestDef};
use chrono::{Duration, Timelike, Utc};
use omnia_guest::Error;
use omnia_guest::api::{Client, Metadata};
use pulse_adapter::{ChangeType, EventType, PulseMessage};

use self::provider::MockProvider;
use crate::provider::{Replay, shift_time};

// Should deserialize XML into a Pulse message.
#[tokio::test]
async fn deserialize_xml() {
    let xml = XmlBuilder::new().xml();
    let message: PulseMessage =
        quick_xml::de::from_reader(xml.as_bytes()).expect("should deserialize");

    let train_update = message.train_update;
    assert_eq!(train_update.train_id(), "5226");
    assert!(!train_update.changes.is_empty());
    assert_eq!(train_update.changes[0].r#type, ChangeType::ArrivedAtStation);
    assert_eq!(train_update.changes[0].station, 0);
}

// Should create an arrival event with a normal stop location.
#[tokio::test]
async fn arrival_event() {
    let file = File::open("data/static/0001.json").expect("should open file");
    let test_def: TestDef<Error> =
        serde_json::from_reader(&file).expect("should deserialize test file");
    let test_case = TestCase::<Replay>::new(test_def).prepare(shift_time);
    let message = test_case.input.as_ref().expect("should have input message").clone();
    let provider = MockProvider::new(test_case);

    let client = Client::new("acme", provider.clone());
    client.call(message, &Metadata::default()).await.expect("should process");

    let events = provider.events();
    assert_eq!(events.len(), 2);

    let event = &events[0];
    assert_eq!(event.event_type, EventType::Location);

    // confirm arrival location is the stop's location
    assert!(event.location_data.latitude.eq(&-36.12345));
    assert!(event.location_data.longitude.eq(&174.12345));
    assert_eq!(event.remote_data.external_id, "vehicle1");
}

// Should create a departure event with an stop location updated.
#[tokio::test]
async fn departure_event() {
    let file = File::open("data/static/0002.json").expect("should open file");
    let test_def: TestDef<Error> =
        serde_json::from_reader(&file).expect("should deserialize test file");
    let test_case = TestCase::<Replay>::new(test_def).prepare(shift_time);
    let message = test_case.input.as_ref().expect("should have input message").clone();
    let provider = MockProvider::new(test_case);

    let client = Client::new("acme", provider.clone());
    client.call(message, &Metadata::default()).await.expect("should process");

    let events = provider.events();
    assert_eq!(events.len(), 2);

    let event = &events[0];
    assert_eq!(event.event_type, EventType::Location);

    // confirm departure location has been updated
    assert!(event.location_data.latitude.eq(&-36.84448));
    assert!(event.location_data.longitude.eq(&174.76915));
}

// Should return no events for an unmapped station.
#[tokio::test]
async fn unmapped_station() {
    let file = File::open("data/static/0003.json").expect("should open file");
    let test_def: TestDef<Error> =
        serde_json::from_reader(&file).expect("should deserialize test file");
    let test_case = TestCase::<Replay>::new(test_def).prepare(shift_time);
    let message = test_case.input.as_ref().expect("should have input message").clone();
    let provider = MockProvider::new(test_case);

    let client = Client::new("acme", provider.clone());
    client.call(message, &Metadata::default()).await.expect("should process");

    let events = provider.events();
    assert!(events.is_empty());
}

// Should return no events when there are no vehicles found for the train id.
#[tokio::test]
async fn no_matching_vehicle() {
    let file = File::open("data/static/0004.json").expect("should open file");
    let test_def: TestDef<Error> =
        serde_json::from_reader(&file).expect("should deserialize test file");
    let test_case = TestCase::<Replay>::new(test_def).prepare(shift_time);
    let message = test_case.input.as_ref().expect("should have input message").clone();
    let provider = MockProvider::new(test_case);

    let client = Client::new("acme", provider.clone());
    client.call(message, &Metadata::default()).await.expect("should process");

    let events = provider.events();
    assert!(events.is_empty());
}

// Should return no events when there are no stop is found for the station.
#[tokio::test]
async fn no_matching_stop() {
    let file = File::open("data/static/0005.json").expect("should open file");
    let test_def: TestDef<Error> =
        serde_json::from_reader(&file).expect("should deserialize test file");
    let test_case = TestCase::<Replay>::new(test_def).prepare(shift_time);
    let message = test_case.input.as_ref().expect("should have input message").clone();
    let provider = MockProvider::new(test_case);

    let client = Client::new("acme", provider.clone());
    client.call(message, &Metadata::default()).await.expect("should process");

    let events = provider.events();
    assert!(events.is_empty());
}

// Should return no events when there is no train update.
#[tokio::test]
async fn no_train_update() {
    expect_error("data/static/0006.json").await;
}

// Should return no events when there is a train update but it contains no
// changes.
#[tokio::test]
async fn no_changes() {
    expect_error("data/static/0007.json").await;
}

// Should return no events when there is a train update but it contains no
// actual changes.
#[tokio::test]
async fn no_actual_changes() {
    expect_error("data/static/0008.json").await;
}

// Should return no events when train update arrives more than 60 seconds after
// the current time.
#[tokio::test]
async fn too_late() {
    expect_error("data/static/0009.json").await;
}

// Should return no events when train update arrives more than 30 seconds
// before the current time.
#[tokio::test]
async fn too_early() {
    expect_error("data/static/0010.json").await;
}

// Run a fixture that is expected to fail and compare the error with the
// fixture's expected output.
async fn expect_error(path: &str) {
    let file = File::open(path).expect("should open file");
    let test_def: TestDef<Error> =
        serde_json::from_reader(&file).expect("should deserialize test file");
    let test_case = TestCase::<Replay>::new(test_def).prepare(shift_time);
    let message = test_case.input.as_ref().expect("should have input message").clone();
    let provider = MockProvider::new(test_case.clone());

    let client = Client::new("acme", provider.clone());

    let Some(expected_result) = &test_case.output else {
        panic!("should have expected output");
    };
    match expected_result {
        Ok(_) => panic!("should have error"),
        Err(expected_error) => {
            let actual_error =
                client.call(message, &Metadata::default()).await.expect_err("should have error");
            assert_eq!(actual_error.code(), expected_error.code());
            assert_eq!(actual_error.description(), expected_error.description());
        }
    }
}

struct XmlBuilder<'a> {
    station: u64,
    vehicle: &'a str,
    arrival: bool,
    delay_secs: i64,
    update: UpdateType,
}

#[derive(PartialEq, Eq)]
#[allow(dead_code)]
enum UpdateType {
    None,
    Full,
    NoChanges,
    NoActualChanges,
}

#[allow(dead_code)]
impl<'a> XmlBuilder<'a> {
    const fn new() -> Self {
        Self {
            station: 0,
            vehicle: "5226",
            arrival: true,
            delay_secs: 0,
            update: UpdateType::Full,
        }
    }

    const fn station(mut self, station: u64) -> Self {
        self.station = station;
        self
    }

    const fn vehicle(mut self, vehicle: &'a str) -> Self {
        self.vehicle = vehicle;
        self
    }

    const fn arrival(mut self, arrival: bool) -> Self {
        self.arrival = arrival;
        self
    }

    const fn delay_secs(mut self, delay_secs: i64) -> Self {
        self.delay_secs = delay_secs;
        self
    }

    const fn update(mut self, update: UpdateType) -> Self {
        self.update = update;
        self
    }

    // Generate a train update XML string.
    fn xml(self) -> String {
        match self.update {
            UpdateType::None => return NO_UPDATE_MSG.to_string(),
            UpdateType::NoChanges => return NO_CHANGE_MSG.to_string(),
            UpdateType::NoActualChanges => return NO_ACTUAL_CHANGE_MSG.to_string(),
            UpdateType::Full => {}
        }

        // create update message
        let now = Utc::now().with_timezone(&TIMEZONE);
        let event_dt = now.sub(Duration::seconds(self.delay_secs));
        let created_date = event_dt.format("%d/%m/%Y");
        let event_secs = event_dt.num_seconds_from_midnight();

        let change_type = if self.arrival { 3 } else { 1 };
        let has_arrived = if self.arrival { "true" } else { "false" };
        let has_departed = if self.arrival { "false" } else { "true" };
        let station = &self.station;
        let vehicle = self.vehicle;

        format!(
            r#"<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="7c104b58-25cb-437a-8c39-297633a6638e" sequence="1214699" xsi:type="CCO">
            <ActualizarDatosTren>
                <trenPar>{vehicle}</trenPar>
                <trenImpar>{vehicle}</trenImpar>
                <fechaCreacion>{created_date}</fechaCreacion>
                <numeroRegistro>9299669</numeroRegistro>
                <operadorComercial>METRO</operadorComercial>
                <pasoTren>
                    <tipoCambio>{change_type}</tipoCambio>
                    <estacion>{station}</estacion>
                    <idPaso>181353261</idPaso>
                    <horaEntrada>{event_secs}</horaEntrada>
                    <horaEntradaReal>{event_secs}</horaEntradaReal>
                    <haEntrado>{has_arrived}</haEntrado>
                    <tipoParada>4</tipoParada>
                    <paridad>p</paridad>
                    <sentido>0</sentido>
                    <horaSalida>{event_secs}</horaSalida>
                    <horaSalidaReal>{event_secs}</horaSalidaReal>
                    <haSalido>{has_departed}</haSalido>
                    <viaEntradaMallas>2</viaEntradaMallas>
                    <retrasoEntrada>-3</retrasoEntrada>
                    <viaCirculacionMallas>2</viaCirculacionMallas>
                    <retrasoSalida>0</retrasoSalida>
                    <horaInicioDetencion>-1</horaInicioDetencion>
                    <duracionDetencion>-1</duracionDetencion>
                </pasoTren>
                <pasoTren>
                    <tipoCambio>3</tipoCambio>
                    <estacion>{station}</estacion>
                    <idPaso>181353261</idPaso>
                    <horaEntrada>58020</horaEntrada>
                    <horaEntradaReal>58017</horaEntradaReal>
                    <haEntrado>true</haEntrado>
                    <tipoParada>4</tipoParada>
                    <paridad>p</paridad>
                    <sentido>0</sentido>
                    <horaSalida>58080</horaSalida>
                    <horaSalidaReal>58080</horaSalidaReal>
                    <haSalido>false</haSalido>
                    <viaEntradaMallas>2</viaEntradaMallas>
                    <retrasoEntrada>-3</retrasoEntrada>
                    <viaCirculacionMallas>2</viaCirculacionMallas>
                    <retrasoSalida>0</retrasoSalida>
                    <horaInicioDetencion>-1</horaInicioDetencion>
                    <duracionDetencion>-1</duracionDetencion>
                </pasoTren>
                <codigoOperadorComercial>-1</codigoOperadorComercial>
                <origenActualizaTren>GAC</origenActualizaTren>
            </ActualizarDatosTren></CCO>"#
        )
    }
}

const NO_UPDATE_MSG: &str = r#"<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="7c104b58-25cb-437a-8c39-297633a6638e" sequence="1214699" xsi:type="CCO">
    </CCO>"#;
const NO_CHANGE_MSG: &str = r#"<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="7c104b58-25cb-437a-8c39-297633a6638e" sequence="1214699" xsi:type="CCO">
    <ActualizarDatosTren>
        <trenPar>5226</trenPar>
        <trenImpar>5226</trenImpar>
    </ActualizarDatosTren>
    </CCO>"#;
const NO_ACTUAL_CHANGE_MSG: &str = r#"<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" stream="7c104b58-25cb-437a-8c39-297633a6638e" sequence="1214699" xsi:type="CCO">
    <ActualizarDatosTren>
        <trenPar>5226</trenPar>
        <trenImpar>5226</trenImpar>
        <pasoTren>
            <tipoCambio>3</tipoCambio>
            <estacion>80</estacion>
            <idPaso>181353261</idPaso>
            <horaEntrada>-1</horaEntrada>
            <horaEntradaReal>-1</horaEntradaReal>
            <haEntrado>false</haEntrado>
            <tipoParada>4</tipoParada>
            <paridad>p</paridad>
            <sentido>0</sentido>
            <horaSalida>58080</horaSalida>
            <horaSalidaReal>58080</horaSalidaReal>
            <haSalido>false</haSalido>
            <viaEntradaMallas>2</viaEntradaMallas>
            <retrasoEntrada>-3</retrasoEntrada>
            <viaCirculacionMallas>2</viaCirculacionMallas>
            <retrasoSalida>0</retrasoSalida>
            <horaInicioDetencion>-1</horaInicioDetencion>
            <duracionDetencion>-1</duracionDetencion>
        </pasoTren>
    </ActualizarDatosTren>
    </CCO>"#;

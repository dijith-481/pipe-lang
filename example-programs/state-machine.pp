// State machine — explicit, typed state transitions
use stdlib.io

type AppState =
  | Idle
  | Loading
  | Ready(str)
  | Failed(str)

type Event =
  | StartLoad
  | DataReceived(str)
  | ErrorOccured(str)
  | Reset

let transition : (AppState, Event) -> AppState
let transition = (state, event) => match (state, event) {
    (Idle,         StartLoad)           => Loading
    (Loading,      DataReceived(data))  => Ready(data)
    (Loading,      ErrorOccured(msg))   => Failed(msg)
    (Ready(_),     Reset)              => Idle
    (Failed(_),    Reset)              => Idle
    (Ready(_),     StartLoad)          => Loading
    (Failed(_),    StartLoad)          => Loading
    _                                  => state  // no-op for invalid transitions
}

let stateToString : (AppState) -> str
let stateToString = (state) => match state {
    Idle          => "Idle"
    Loading       => "Loading..."
    Ready(data)   => "Ready: " ++ data
    Failed(msg)   => "Failed: " ++ msg
}

let main : () -> Effect<Unit>
let main = do {
    // Simulate a series of events
    let events = [StartLoad, DataReceived("user data"), Reset, StartLoad, ErrorOccured("timeout"), Reset]

    // Fold events through the state machine
    let finalState = events.fold(Idle, (state, event) => transition(state, event))

    IO.println("Final state: " ++ stateToString(finalState))

    // Show each transition
    IO.println("")
    IO.println("Transitions:")
    let states = events.fold([Idle], (acc, event) => {
        let last = acc[acc.len() - 1]
        let next = transition(last, event)
        acc ++ [next]
    })
    states.map((s) => IO.println("  -> " ++ stateToString(s)))
}

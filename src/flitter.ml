let usage = 
  "Usage:\n" ^

  "flitter <splits_path>\n" ^
  "Open the splits file pointed to by `splits_path`.\n"

let run_event_loop timer =
  let%lwt event_loop = Event_loop.make timer in
  Event_loop.loop event_loop

let run () =
  match Sys.argv with
  | [|_; path|] ->
    let timer = Loadsave.load path in
    Lwt_main.run (run_event_loop timer)

  | _ -> print_string usage
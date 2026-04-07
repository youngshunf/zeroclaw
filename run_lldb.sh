#!/bin/bash
BIN=$(cargo test e2e_failed_non_retryable_turn_does_not --features huanxing --no-run --message-format=json | grep -v 'crates' | grep '"executable":' | grep -o 'target[^"]*' | head -n 1)
lldb --batch -o "run" -o "bt all" -o "quit" -- $BIN e2e_failed_non_retryable_turn_does_not --exact

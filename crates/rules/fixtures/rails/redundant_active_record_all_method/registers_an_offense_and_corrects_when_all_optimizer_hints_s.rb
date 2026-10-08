User.all.optimizer_hints("SeqScan(users)", "Parallel(users 8)")
     ^^^ Redundant `all` detected.

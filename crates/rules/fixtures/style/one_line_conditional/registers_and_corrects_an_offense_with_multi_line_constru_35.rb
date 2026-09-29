map { |line| if line.match(/^ *#/) || line.strip.empty? then next else line end }
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Favor multi-line `if` over single-line `if/then/else/end` constructs.

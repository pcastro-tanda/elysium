map { |line| if line.match(/^ *#/) || line.strip.empty? then next else line end }
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Favor the ternary operator (`?:`) over single-line `if/then/else/end` constructs.

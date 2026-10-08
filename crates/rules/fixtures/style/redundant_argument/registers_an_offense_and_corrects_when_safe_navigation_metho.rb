foo&.join('')
         ^^^^ Argument '' is redundant because it is implied by default.
foo&.sum(0)
        ^^^ Argument 0 is redundant because it is implied by default.
foo&.split(' ')
          ^^^^^ Argument ' ' is redundant because it is implied by default.
foo&.chomp("\n")
          ^^^^^^ Argument "\n" is redundant because it is implied by default.
foo&.chomp!("\n")
           ^^^^^^ Argument "\n" is redundant because it is implied by default.

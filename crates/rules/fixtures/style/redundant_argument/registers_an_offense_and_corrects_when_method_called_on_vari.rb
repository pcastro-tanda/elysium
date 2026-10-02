foo.join('')
        ^^^^ Argument '' is redundant because it is implied by default.
foo.sum(0)
       ^^^ Argument 0 is redundant because it is implied by default.
exit(true)
    ^^^^^^ Argument true is redundant because it is implied by default.
exit!(false)
     ^^^^^^^ Argument false is redundant because it is implied by default.
foo.to_i(10)
        ^^^^ Argument 10 is redundant because it is implied by default.
foo.split(' ')
         ^^^^^ Argument ' ' is redundant because it is implied by default.
foo.chomp("\n")
         ^^^^^^ Argument "\n" is redundant because it is implied by default.
foo.chomp!("\n")
          ^^^^^^ Argument "\n" is redundant because it is implied by default.

x = "ABC".gsub(/(A)(B)(C)/, "D#{'\2'}F")
                                ^^^^ Literal interpolation detected.
"this is #{'\n'} silly"
           ^^^^ Literal interpolation detected.
"this is #{%q(\n)} silly"
           ^^^^^^ Literal interpolation detected.

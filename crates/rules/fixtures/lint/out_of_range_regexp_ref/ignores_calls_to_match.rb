/(foo)(bar)/.match("foobar")
/(foo)(bar)(baz)/.match?("foobarbaz")
puts $3
     ^^ $3 is out of range (2 regexp capture groups detected).

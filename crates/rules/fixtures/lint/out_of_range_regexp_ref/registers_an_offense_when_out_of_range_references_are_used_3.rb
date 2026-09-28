/(?<foo>FOO)(BAR)/ =~ "FOOBAR"
puts $2
     ^^ $2 is out of range (1 regexp capture group detected).

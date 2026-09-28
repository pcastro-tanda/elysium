/(?<foo>FOO)(?<bar>BAR)/ =~ "FOOBAR"
puts $3
     ^^ $3 is out of range (2 regexp capture groups detected).

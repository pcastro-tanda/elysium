%w[foo foobar].grep(/(foo)/) { $2 }
                               ^^ $2 is out of range (1 regexp capture group detected).

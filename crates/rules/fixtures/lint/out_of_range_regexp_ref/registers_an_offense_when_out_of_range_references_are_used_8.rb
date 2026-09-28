case "foobar"
in /(foo)(bar)/ | "foo"
  $3
  ^^ $3 is out of range (2 regexp capture groups detected).
end

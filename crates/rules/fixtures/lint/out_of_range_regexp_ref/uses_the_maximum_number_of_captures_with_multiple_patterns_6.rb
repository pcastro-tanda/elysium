case "foobar"
in /(foo)baz/ | /(foo)(bar)/
  $3
  ^^ $3 is out of range (2 regexp capture groups detected).
end

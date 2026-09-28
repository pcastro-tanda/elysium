case "foobarbaz"
when /(foo)(bar)/, /(bar)baz/
  $3
  ^^ $3 is out of range (2 regexp capture groups detected).
end

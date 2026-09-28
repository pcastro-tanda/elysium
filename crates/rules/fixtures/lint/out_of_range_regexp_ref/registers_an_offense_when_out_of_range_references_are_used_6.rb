a = 1
case array
in [^a, /(foo)(bar)/, /(foo)bar/]
  $3
  ^^ $3 is out of range (2 regexp capture groups detected).
end

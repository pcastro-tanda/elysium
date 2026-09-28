case array
in [/(foo)(bar)/, /#{var}/]
  $3
  ^^ $3 is out of range (2 regexp capture groups detected).
end

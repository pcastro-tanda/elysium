case array
in [/(foo)(bar)/, /(bar)baz/] => x
  $3
  ^^ $3 is out of range (2 regexp capture groups detected).
end

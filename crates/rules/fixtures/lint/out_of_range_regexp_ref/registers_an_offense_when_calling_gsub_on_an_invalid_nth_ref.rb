if "some : line " =~ / : (.+)/
  $2.gsub(/ {2}/, " ")
  ^^ $2 is out of range (1 regexp capture group detected).
end

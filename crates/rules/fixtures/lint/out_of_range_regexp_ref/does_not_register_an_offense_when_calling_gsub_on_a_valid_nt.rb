if "some : line " =~ / : (.+)/
  $1.gsub(/ {2}/, " ")
end

"some : line " =~ / : (.+)/
$1.gsub(/ {2}/, " ")
puts $1
     ^^ $1 is out of range (no regexp capture groups detected).

var = '(\d+)'
/(?<foo>#{var}*)/ =~ "12"
puts $1
puts $2

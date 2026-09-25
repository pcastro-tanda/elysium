string_value = '10'
string_value.to_i
^^^^^^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `string_value.to_i`, use stricter `Integer(string_value, 10)`.

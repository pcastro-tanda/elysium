"10"&.to_i
^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `"10".to_i`, use stricter `Integer("10", 10)`.

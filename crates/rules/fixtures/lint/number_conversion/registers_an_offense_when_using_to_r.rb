"1/3".to_r
^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `"1/3".to_r`, use stricter `Rational("1/3")`.

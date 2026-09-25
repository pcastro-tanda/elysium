foo&.to_i
^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `foo&.to_i`, use stricter `Integer(foo, 10)`.

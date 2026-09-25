foo&.bar.to_i
^^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `foo&.bar.to_i`, use stricter `Integer(foo&.bar, 10)`.

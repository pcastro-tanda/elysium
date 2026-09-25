10.hours.to_i
^^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `10.hours.to_i`, use stricter `Integer(10.hours, 10)`.

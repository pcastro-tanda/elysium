"1,2,3,foo,5,6,7,8".split(',')&.map(&:to_i)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `&:to_i`, use stricter `{ |i| Integer(i, 10) }`.

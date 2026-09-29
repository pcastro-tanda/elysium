# example.rb
case
^^^^ Do not use empty `case` condition, instead use an `if` expression.
when object.nil?
  Object.new
else
  object
end

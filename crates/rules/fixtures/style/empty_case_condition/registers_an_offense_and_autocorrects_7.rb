case
^^^^ Do not use empty `case` condition, instead use an `if` expression.
when false then foo
when nil, false, 1 then bar
when false, 1 then baz
end

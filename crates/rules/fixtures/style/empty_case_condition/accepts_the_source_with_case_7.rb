case :a
when false then foo
when nil, false, 1 then bar
when false, 1 then baz
end

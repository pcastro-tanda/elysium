$some_value ||= begin
  if rand(1..2).odd?
    "odd number"
  else
    "even number"
  end
end

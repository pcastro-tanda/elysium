(1..4).reduce(0) do |acc, el|
  <<~RESULT
    #{acc}#{el}
  RESULT
end

(1..4).inject(0) do |acc, el|
  <<~RESULT
    #{acc}#{el}
  RESULT
end

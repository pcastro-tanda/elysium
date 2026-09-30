if line.is_a?(String)
  expect(actual[ix]).to eq(line)
else
  expect(actual[ix]).to match(line)
end

it 'does something' do
  @n = do_something
  expect(@n).must_be_within_delta 42
end

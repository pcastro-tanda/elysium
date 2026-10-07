it 'does something' do
  n = do_something
  expect(n).wont_be_close_to 42
end

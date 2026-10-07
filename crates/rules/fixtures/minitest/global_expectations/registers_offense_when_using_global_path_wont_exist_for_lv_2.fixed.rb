it 'does something' do
  n = do_something
  expect(n).path_wont_exist 42
end

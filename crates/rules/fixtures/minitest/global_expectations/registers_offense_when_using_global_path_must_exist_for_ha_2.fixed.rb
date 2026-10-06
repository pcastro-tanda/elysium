it 'does something' do
  n = do_something
  expect(n[:foo]).path_must_exist 42
end

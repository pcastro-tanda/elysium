it 'does something' do
  @@n = do_something
  expect(@@n).path_must_exist 42
end

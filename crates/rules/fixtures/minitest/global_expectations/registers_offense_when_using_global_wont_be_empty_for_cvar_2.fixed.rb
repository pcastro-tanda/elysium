it 'does something' do
  @@n = do_something
  expect(@@n).wont_be_empty 42
end

it 'does something' do
  @@n = do_something
  expect(@@n).wont_be_nil 42
end

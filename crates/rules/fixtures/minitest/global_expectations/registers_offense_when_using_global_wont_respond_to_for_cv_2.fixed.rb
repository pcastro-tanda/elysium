it 'does something' do
  @@n = do_something
  expect(@@n).wont_respond_to 42
end

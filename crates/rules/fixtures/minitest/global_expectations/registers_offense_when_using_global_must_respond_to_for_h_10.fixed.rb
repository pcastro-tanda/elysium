it 'does something' do
  @@n = do_something
  expect(@@n[:foo]).must_respond_to 42
end

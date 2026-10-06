it 'does something' do
  @@n = do_something
  _(@@n).must_respond_to 42
end

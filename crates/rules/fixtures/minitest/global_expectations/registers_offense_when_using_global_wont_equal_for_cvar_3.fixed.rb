it 'does something' do
  @@n = do_something
  _(@@n).wont_equal 42
end

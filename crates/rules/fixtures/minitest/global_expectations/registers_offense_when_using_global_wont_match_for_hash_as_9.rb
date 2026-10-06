it 'does something' do
  @@n = do_something
  @@n[:foo].wont_match 42
  ^^^^^^^^^ Use `_(@@n[:foo])` instead.
end

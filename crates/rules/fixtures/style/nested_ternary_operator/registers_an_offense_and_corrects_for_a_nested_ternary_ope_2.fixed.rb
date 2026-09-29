if cond
foo
else
bar(foo.a ? foo.b : foo) { |e, k| e.nil? ? nil : e[k] }
end

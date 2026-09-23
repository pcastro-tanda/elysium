def method
  Pry.rescue { puts 1 }
  ^^^^^^^^^^ Remove debugger entry point `Pry.rescue`.
  ::Pry.rescue { puts 1 }
  ^^^^^^^^^^^^ Remove debugger entry point `::Pry.rescue`.
end

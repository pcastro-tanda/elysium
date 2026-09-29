if (x > 10)
   ^^^^^^^^ Don't use parentheses around the condition of an `if`.
elsif (x < 3)
      ^^^^^^^ Don't use parentheses around the condition of an `elsif`.
end
unless (x > 10)
       ^^^^^^^^ Don't use parentheses around the condition of an `unless`.
end
while (x > 10)
      ^^^^^^^^ Don't use parentheses around the condition of a `while`.
end
until (x > 10)
      ^^^^^^^^ Don't use parentheses around the condition of an `until`.
end
x += 1 if (x < 10)
          ^^^^^^^^ Don't use parentheses around the condition of an `if`.
x += 1 unless (x < 10)
              ^^^^^^^^ Don't use parentheses around the condition of an `unless`.
x += 1 until (x < 10)
             ^^^^^^^^ Don't use parentheses around the condition of an `until`.
x += 1 while (x < 10)
             ^^^^^^^^ Don't use parentheses around the condition of a `while`.

[].inject({}) { $GLOBAL[rand] = rand; $GLOBAL }

[].reduce({}) do
   $GLOBAL[rand] = rand
   $GLOBAL
end

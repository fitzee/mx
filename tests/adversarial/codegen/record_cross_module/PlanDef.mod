IMPLEMENTATION MODULE PlanDef;

PROCEDURE InitPlan(VAR p: Plan);
BEGIN
  p.tag := 5;
  p.proj.count := 0;
  p.proj.hasProject := FALSE;
  p.limit := 100
END InitPlan;

END PlanDef.

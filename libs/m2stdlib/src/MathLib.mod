IMPLEMENTATION MODULE MathLib;
FROM CMath IMPORT sqrtf, sinf, cosf, expf, logf, atanf, floorf;
FROM CRand IMPORT rand, srand;

CONST RandMax = 2147483647;

PROCEDURE sqrt(x: REAL): REAL;
BEGIN RETURN sqrtf(x) END sqrt;

PROCEDURE sin(x: REAL): REAL;
BEGIN RETURN sinf(x) END sin;

PROCEDURE cos(x: REAL): REAL;
BEGIN RETURN cosf(x) END cos;

PROCEDURE exp(x: REAL): REAL;
BEGIN RETURN expf(x) END exp;

PROCEDURE ln(x: REAL): REAL;
BEGIN RETURN logf(x) END ln;

PROCEDURE arctan(x: REAL): REAL;
BEGIN RETURN atanf(x) END arctan;

PROCEDURE entier(x: REAL): INTEGER;
BEGIN RETURN INTEGER(floorf(x)) END entier;

PROCEDURE Random(): REAL;
BEGIN
    RETURN REAL(rand()) / (REAL(RandMax) + 1.0)
END Random;

PROCEDURE Randomize(seed: CARDINAL);
BEGIN
    srand(seed)
END Randomize;

END MathLib.

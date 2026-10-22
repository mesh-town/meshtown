/* Navigation to an address goes through a replaceable `loc`, so the code doesn't touch the global window object. */

export function navigateTo(url: string, loc: Pick<Location, "assign"> = location): void {
  loc.assign(url);
}

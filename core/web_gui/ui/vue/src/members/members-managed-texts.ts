/* Menu labels and dialog texts of a managed network. */

import { forDevice } from "../common/device.ts";

export const MENU_MAKE_ADMIN = "Make admin";
export const MENU_MAKE_MEMBER = "Make regular member";
export const MENU_GIVE_UP = "Give up admin rights";
export const MENU_GIVE_UP_ONLY = "Give up admin rights (you're the only admin)";
export const MENU_REMOVE = "Remove from network…";
export const MENU_REMOVE_ADMINS_ONLY = "Remove from network (admins only)";
export const MENU_DELETE_NETWORK = "Delete network for everyone…";
export const TITLE_ADMINS_ONLY = "Only admins can do this";
export const TITLE_CREATOR = "The creator of the network stays an admin";
export const TITLE_ONLY_ADMIN = "Make another member an admin first";

export const makeAdminTitle = (name: string, net: string): string => "Make " + name + ' an admin of "' + net + '"?';
export const makeAdminBody = (name: string): string =>
  name + " will be able to invite and remove members, make others admins and delete the network.";
export const giveUpTitle = (net: string): string => 'Give up admin rights in "' + net + '"?';
export const GIVE_UP_BODY = "You stay a member. Only another admin can make you an admin again.";
export const removeTitle = (name: string, net: string): string => "Remove " + name + ' from "' + net + '"?';
export const removeBody = (name: string): string =>
  name + " is disconnected from all members right away. To come back, it needs a new invite from an admin.";
export const deleteNetTitle = (net: string): string => "Delete \"" + net + "\" for everyone?";
export const deleteNetBody = (members: number, net: string): string =>
  "All " + members + ' members leave "' + net + forDevice("\", this computer too. This can't be undone: to be together again, someone has to create a new network and invite everyone.");
export const deleteNetExitLine = (exit: string): string =>
  "Your exit " + exit + " is in this network. After the network is deleted, sites will open directly until you choose another exit.";
export const DELETE_NET_FIELD = "Type the network name to confirm";
export const lastAdminTitle = (net: string): string => "You're the only admin of \"" + net + '"';
export const LAST_ADMIN_BODY =
  "If you leave, nobody can invite or remove members there. Make another member an admin first, or delete the network for everyone.";
